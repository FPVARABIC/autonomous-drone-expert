import {
  WasmReadonlySerialDirective,
  WasmReadonlySerialDiscovery,
} from "virtual:ade-web-readonly-serial-wasm";

import {
  DiagnosticTraceRecorder,
  formatSafeDiagnosticTrace,
} from "../diagnostics/readonly-trace.mjs";

const INITIAL_MSP_BAUD_RATE = 115200;
const DEFAULT_EXCHANGE_TIMEOUT_MS = 1500;
const MAX_CHUNKS_PER_EXCHANGE = 128;

function stableFailure(error, fallback = "Unknown") {
  switch (error?.name) {
    case "NotFoundError":
    case "AbortError":
      return "Cancelled";
    case "NotAllowedError":
    case "SecurityError":
      return "PermissionDenied";
    case "InvalidStateError":
      return "PortBusy";
    case "NetworkError":
      return "Disconnected";
    default:
      return fallback;
  }
}

function timeoutRead(reader, milliseconds) {
  let timer;
  return Promise.race([
    reader.read().finally(() => clearTimeout(timer)),
    new Promise((_, reject) => {
      timer = setTimeout(() => {
        const error = new Error("bounded serial response timeout");
        error.name = "TimeoutError";
        reject(error);
      }, milliseconds);
    }),
  ]);
}

function optionalFields(fields) {
  return Object.fromEntries(
    Object.entries(fields).filter(([, value]) => value !== undefined && value !== null),
  );
}

/**
 * Narrow host for Rust-issued read-only discovery directives.
 *
 * The selected port and diagnostic trace are memory-only. This class never enumerates grants,
 * reads USB metadata, logs frames, constructs MSP, or accepts a caller-selected command.
 */
export class WebSerialReadonlyHost {
  #serial;
  #selectedPort = null;
  #opened = false;
  #reader = null;
  #readerCancelled = false;
  #writer = null;
  #timeoutMs;
  #trace = new DiagnosticTraceRecorder();
  #activeStage;
  #activeCommand;
  #terminalOrigin;

  constructor({
    serial = globalThis.navigator?.serial,
    timeoutMs = DEFAULT_EXCHANGE_TIMEOUT_MS,
  } = {}) {
    this.#serial = serial;
    this.#timeoutMs = timeoutMs;
  }

  #recordFinalFailure(failureClass, origin) {
    this.#terminalOrigin = origin;
    this.#trace.record({
      layer: "HOST",
      phase: "FINAL_RESULT",
      event: "FINAL_FAILED",
      failureClass,
      origin,
    });
  }

  #failedResult(failure, failureOrigin) {
    return {
      outcome: "failed",
      failure,
      failureOrigin,
      hardwareObserved: false,
    };
  }

  #drainRustTrace(discovery) {
    let event;
    while ((event = discovery.takeTraceEvent())) {
      try {
        const candidate = optionalFields({
          layer: event.layer,
          phase: event.phase,
          event: event.event,
          stage: event.stage,
          command: event.command,
          byteCount: event.byteCount,
          direction: event.direction,
          failureClass: event.failureClass,
          failureReason: event.failureReason,
          origin: event.origin,
        });
        this.#trace.record(candidate);
        if (candidate.event === "DIRECTIVE") {
          this.#activeStage = candidate.stage;
          this.#activeCommand = candidate.command;
        }
        if (candidate.origin) this.#terminalOrigin = candidate.origin;
      } finally {
        event.free();
      }
    }
  }

  #recordSerialFailure(event, failureClass, origin) {
    this.#terminalOrigin = origin;
    this.#trace.record({
      layer: "SERIAL",
      phase: event === "TX_FAILED" ? "SERIAL_WRITE" : "SERIAL_READ",
      event,
      ...optionalFields({
        stage: this.#activeStage,
        command: this.#activeCommand,
        direction: event === "TX_FAILED" ? "REQUEST" : undefined,
      }),
      failureClass,
      origin,
    });
  }

  /** Must be called directly from the owner-controlled user gesture handler. */
  async selectPortFromUserGesture() {
    this.#trace.beginAttempt();
    this.#activeStage = undefined;
    this.#activeCommand = undefined;
    this.#terminalOrigin = undefined;
    this.#trace.record({
      layer: "UI",
      phase: "PORT_SELECTION",
      event: "SELECT_START",
    });

    if (!this.#serial?.requestPort) {
      this.#trace.record({
        layer: "UI",
        phase: "PORT_SELECTION",
        event: "SELECT_FAILED",
        failureClass: "Unavailable",
        origin: "PORT_SELECTION",
      });
      this.#recordFinalFailure("Unavailable", "PORT_SELECTION");
      return { ok: false, failure: "Unavailable", failureOrigin: "PORT_SELECTION" };
    }

    try {
      this.#selectedPort = await this.#serial.requestPort();
      this.#trace.record({
        layer: "UI",
        phase: "PORT_SELECTION",
        event: "SELECT_OK",
      });
      return { ok: true };
    } catch (error) {
      const failure = stableFailure(error);
      this.#selectedPort = null;
      this.#trace.record({
        layer: "UI",
        phase: "PORT_SELECTION",
        event: "SELECT_FAILED",
        failureClass: failure,
        origin: "PORT_SELECTION",
      });
      this.#recordFinalFailure(failure, "PORT_SELECTION");
      return { ok: false, failure, failureOrigin: "PORT_SELECTION" };
    }
  }

  async #cleanup() {
    let firstFailure = null;
    this.#trace.record({
      layer: "CLEANUP",
      phase: "CLEANUP",
      event: "CLEANUP_START",
    });

    const rememberFailure = (failure, origin) => {
      firstFailure ??= { failure, origin };
      this.#trace.record({
        layer: "CLEANUP",
        phase: "CLEANUP",
        event: "CLEANUP_FAILED",
        failureClass: failure,
        origin,
      });
    };

    if (this.#reader) {
      if (!this.#readerCancelled) {
        try {
          await this.#reader.cancel();
          this.#readerCancelled = true;
        } catch (error) {
          rememberFailure(stableFailure(error), "READER_CANCEL");
        }
      }
      try {
        this.#reader.releaseLock();
      } catch (error) {
        rememberFailure(stableFailure(error), "READER_RELEASE");
      }
      this.#reader = null;
      this.#readerCancelled = false;
    }

    if (this.#writer) {
      try {
        this.#writer.releaseLock();
      } catch (error) {
        rememberFailure(stableFailure(error), "WRITER_RELEASE");
      }
      this.#writer = null;
    }

    if (this.#opened && this.#selectedPort) {
      this.#trace.record({
        layer: "HOST",
        phase: "PORT_CLOSE",
        event: "PORT_CLOSE_START",
      });
      try {
        await this.#selectedPort.close();
        this.#trace.record({
          layer: "HOST",
          phase: "PORT_CLOSE",
          event: "PORT_CLOSE_OK",
        });
      } catch (error) {
        const failure = stableFailure(error);
        firstFailure ??= { failure, origin: "PORT_CLOSE" };
        this.#trace.record({
          layer: "HOST",
          phase: "PORT_CLOSE",
          event: "PORT_CLOSE_FAILED",
          failureClass: failure,
          origin: "PORT_CLOSE",
        });
      }
    }

    this.#opened = false;
    this.#selectedPort = null;
    if (!firstFailure) {
      this.#trace.record({
        layer: "CLEANUP",
        phase: "CLEANUP",
        event: "CLEANUP_OK",
      });
    }
    return firstFailure;
  }

  #acceptExchangeFailure(discovery, directive, failure, origin, event) {
    this.#recordSerialFailure(event, failure, origin);
    const next = discovery.acceptExchangeFailure(directive.requestId, failure);
    this.#drainRustTrace(discovery);
    return next;
  }

  async #exchange(discovery, directive) {
    if (!this.#activeStage || !this.#activeCommand) {
      throw new TypeError("RUST_WEB_SERIAL_REFUSAL:MISSING_TRACE_DIRECTIVE");
    }

    if (!this.#writer) {
      try {
        this.#writer = this.#selectedPort?.writable?.getWriter();
      } catch (error) {
        return this.#acceptExchangeFailure(
          discovery,
          directive,
          stableFailure(error, "Disconnected"),
          "WRITER_ACQUISITION",
          "TX_FAILED",
        );
      }
    }
    if (!this.#writer) {
      return this.#acceptExchangeFailure(
        discovery,
        directive,
        "Disconnected",
        "WRITER_ACQUISITION",
        "TX_FAILED",
      );
    }

    if (!this.#reader) {
      try {
        this.#reader = this.#selectedPort?.readable?.getReader();
      } catch (error) {
        return this.#acceptExchangeFailure(
          discovery,
          directive,
          stableFailure(error, "Disconnected"),
          "READER_ACQUISITION",
          "RX_FAILED",
        );
      }
    }
    if (!this.#reader) {
      return this.#acceptExchangeFailure(
        discovery,
        directive,
        "Disconnected",
        "READER_ACQUISITION",
        "RX_FAILED",
      );
    }

    const bytes = directive.bytes;
    this.#trace.record({
      layer: "SERIAL",
      phase: "SERIAL_WRITE",
      event: "TX_START",
      stage: this.#activeStage,
      command: this.#activeCommand,
      byteCount: bytes.byteLength,
      direction: "REQUEST",
    });
    try {
      await this.#writer.write(bytes);
      this.#trace.record({
        layer: "SERIAL",
        phase: "SERIAL_WRITE",
        event: "TX_OK",
        stage: this.#activeStage,
        command: this.#activeCommand,
        byteCount: bytes.byteLength,
        direction: "REQUEST",
      });
    } catch (error) {
      return this.#acceptExchangeFailure(
        discovery,
        directive,
        stableFailure(error, "Disconnected"),
        "SERIAL_WRITE",
        "TX_FAILED",
      );
    }

    for (let chunks = 0; chunks < MAX_CHUNKS_PER_EXCHANGE; chunks += 1) {
      let read;
      try {
        read = await timeoutRead(this.#reader, this.#timeoutMs);
      } catch (error) {
        if (error?.name === "TimeoutError") {
          try {
            await this.#reader.cancel();
            this.#readerCancelled = true;
          } catch (cancelError) {
            this.#trace.record({
              layer: "CLEANUP",
              phase: "CLEANUP",
              event: "CLEANUP_FAILED",
              failureClass: stableFailure(cancelError),
              origin: "READER_CANCEL",
            });
          }
          return this.#acceptExchangeFailure(
            discovery,
            directive,
            "Timeout",
            "SERIAL_TIMEOUT",
            "RX_FAILED",
          );
        }
        return this.#acceptExchangeFailure(
          discovery,
          directive,
          stableFailure(error, "Disconnected"),
          "SERIAL_READ",
          "RX_FAILED",
        );
      }

      const { value, done } = read;
      if (done || !(value instanceof Uint8Array)) {
        return this.#acceptExchangeFailure(
          discovery,
          directive,
          "Disconnected",
          "SERIAL_READ",
          "RX_FAILED",
        );
      }
      this.#trace.record({
        layer: "SERIAL",
        phase: "SERIAL_READ",
        event: "RX_CHUNK",
        stage: this.#activeStage,
        command: this.#activeCommand,
        byteCount: value.byteLength,
      });
      const next = discovery.acceptReadChunk(directive.requestId, value);
      this.#drainRustTrace(discovery);
      if (next) return next;
    }

    return this.#acceptExchangeFailure(
      discovery,
      directive,
      "Timeout",
      "SERIAL_TIMEOUT",
      "RX_FAILED",
    );
  }

  /** Create and run the exact generated Rust state machine after explicit selection. */
  async discover() {
    if (!this.#selectedPort) {
      this.#trace.beginAttempt();
      this.#trace.record({
        layer: "HOST",
        phase: "DISCOVERY",
        event: "DISCOVERY_START",
      });
      this.#recordFinalFailure("Unavailable", "DISCOVERY");
      return this.#failedResult("Unavailable", "DISCOVERY");
    }

    let discovery;
    try {
      discovery = new WasmReadonlySerialDiscovery();
    } catch {
      await this.#cleanup();
      this.#recordFinalFailure("Unknown", "DIRECTIVE_REFUSAL");
      return this.#failedResult("Unknown", "DIRECTIVE_REFUSAL");
    }
    this.#trace.record({
      layer: "HOST",
      phase: "DISCOVERY",
      event: "DISCOVERY_START",
    });
    try {
      let directive = discovery.begin();
      while (directive) {
        if (!(directive instanceof WasmReadonlySerialDirective)) {
          throw new TypeError("RUST_WEB_SERIAL_REFUSAL:UNTRUSTED_DIRECTIVE");
        }
        const current = directive;
        try {
          switch (current.kind) {
            case "open-selected-read-only-port":
              this.#trace.record({
                layer: "HOST",
                phase: "PORT_OPEN",
                event: "PORT_OPEN_START",
              });
              try {
                await this.#selectedPort.open({ baudRate: INITIAL_MSP_BAUD_RATE });
                this.#opened = true;
                this.#trace.record({
                  layer: "HOST",
                  phase: "PORT_OPEN",
                  event: "PORT_OPEN_OK",
                });
                directive = discovery.acceptOpenSuccess(current.requestId);
                this.#drainRustTrace(discovery);
              } catch (error) {
                const failure = stableFailure(error, "Unknown");
                this.#terminalOrigin = "PORT_OPEN";
                this.#trace.record({
                  layer: "HOST",
                  phase: "PORT_OPEN",
                  event: "PORT_OPEN_FAILED",
                  failureClass: failure,
                  origin: "PORT_OPEN",
                });
                discovery.acceptOpenFailure(current.requestId, failure);
                directive = null;
              }
              break;
            case "exchange-identification-read":
              directive = await this.#exchange(discovery, current);
              break;
            case "close": {
              const closeFailure = await this.#cleanup();
              if (closeFailure) {
                this.#terminalOrigin = closeFailure.origin;
                discovery.acceptCloseFailure(current.requestId, closeFailure.failure);
              } else {
                discovery.acceptCloseSuccess(current.requestId);
              }
              directive = null;
              break;
            }
            default:
              throw new Error("RUST_WEB_SERIAL_REFUSAL:UNKNOWN_DIRECTIVE_KIND");
          }
        } finally {
          current.free();
        }
      }

      if (this.#selectedPort || this.#opened || this.#reader || this.#writer) {
        const cleanupFailure = await this.#cleanup();
        if (cleanupFailure) this.#terminalOrigin = cleanupFailure.origin;
      }

      const result = {
        outcome: discovery.outcomeKind,
        failure: discovery.failureClass ?? undefined,
        failureOrigin: discovery.failureClass
          ? (this.#terminalOrigin ?? "FINAL_RESULT")
          : undefined,
        failureStage: discovery.failureStage ?? undefined,
        failureReason: discovery.failureReason ?? undefined,
        scopeMismatchField: discovery.scopeMismatchField ?? undefined,
        apiVersion: discovery.apiVersion ?? undefined,
        fcVariant: discovery.fcVariant ?? undefined,
        fcVersion: discovery.fcVersion ?? undefined,
        targetName: discovery.targetName ?? undefined,
        readProfileId: discovery.readProfileId ?? undefined,
        readProfileWriteAuthority: discovery.readProfileWriteAuthority ?? undefined,
        capabilityStatus: discovery.capabilityStatus ?? undefined,
        capabilityPackId: discovery.capabilityPackId ?? undefined,
        capabilityTrust: discovery.capabilityTrust ?? undefined,
        capabilityWritePolicy: discovery.capabilityWritePolicy ?? undefined,
        hardwareObserved: discovery.hardwareObserved,
      };
      if (result.outcome === "failed") {
        this.#recordFinalFailure(
          result.failure ?? "Unknown",
          result.failureOrigin ?? "FINAL_RESULT",
        );
      } else {
        this.#trace.record({
          layer: "HOST",
          phase: "FINAL_RESULT",
          event: "FINAL_OK",
        });
      }
      return result;
    } catch {
      await this.#cleanup();
      this.#recordFinalFailure("Unknown", "DIRECTIVE_REFUSAL");
      return this.#failedResult("Unknown", "DIRECTIVE_REFUSAL");
    } finally {
      if (this.#selectedPort || this.#opened || this.#reader || this.#writer) {
        await this.#cleanup();
      }
      discovery.free();
    }
  }

  recordUiBoundaryFailure() {
    this.#trace.record({
      layer: "UI",
      phase: "UI_BOUNDARY",
      event: "UI_BOUNDARY_FAILED",
      failureClass: "Unknown",
      origin: "UI_BOUNDARY",
    });
    this.#recordFinalFailure("Unknown", "UI_BOUNDARY");
  }

  recordHardwareEvidenceBoundary() {
    this.#recordFinalFailure("HardwareEvidenceBoundary", "FINAL_RESULT");
  }

  diagnosticTrace() {
    return this.#trace.snapshot();
  }

  safeDiagnosticTraceText() {
    return formatSafeDiagnosticTrace([...this.#trace.snapshot()]);
  }

  clearDiagnosticTrace() {
    this.#trace.clear();
  }
}

export const WEB_SERIAL_READONLY_INITIAL_BAUD_RATE = INITIAL_MSP_BAUD_RATE;
