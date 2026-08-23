import initReadonlySerialWasm from "virtual:ade-web-readonly-serial-wasm";

import { WebSerialReadonlyHost } from "../transport/webserial-readonly-host.mjs";

function privacyBoundedResult(result, host) {
  if (result.hardwareObserved !== false) {
    host.recordHardwareEvidenceBoundary();
    return {
      outcome: "failed",
      failure: "HardwareEvidenceBoundary",
      failureOrigin: "FINAL_RESULT",
    };
  }
  return {
    outcome: result.outcome,
    apiVersion: result.apiVersion,
    fcVariant: result.fcVariant,
    fcVersion: result.fcVersion,
    targetName: result.targetName,
    scopeMismatchField: result.scopeMismatchField,
    failure: result.failure,
    failureOrigin: result.failureOrigin,
    failureStage: result.failureStage,
    failureReason: result.failureReason,
    readProfileId: result.readProfileId,
    readProfileWriteAuthority: result.readProfileWriteAuthority,
    capabilityStatus: result.capabilityStatus,
    capabilityPackId: result.capabilityPackId,
    capabilityTrust: result.capabilityTrust,
    capabilityWritePolicy: result.capabilityWritePolicy,
  };
}

class PreparedReadonlyFcConnection {
  #host = new WebSerialReadonlyHost();

  async selectPortFromUserGesture() {
    const selection = await this.#host.selectPortFromUserGesture();
    return selection;
  }

  async discover() {
    try {
      return privacyBoundedResult(await this.#host.discover(), this.#host);
    } catch {
      this.#host.recordUiBoundaryFailure();
      return {
        outcome: "failed",
        failure: "Unknown",
        failureOrigin: "UI_BOUNDARY",
      };
    }
  }

  recordUiBoundaryFailure() {
    this.#host.recordUiBoundaryFailure();
  }

  diagnosticTrace() {
    return this.#host.diagnosticTrace();
  }

  safeDiagnosticTraceText() {
    return this.#host.safeDiagnosticTraceText();
  }

  clearDiagnosticTrace() {
    this.#host.clearDiagnosticTrace();
  }
}

/** Prepare the audited Rust runtime before the product enables either connection button. */
export async function prepareReadonlyFcConnection() {
  await initReadonlySerialWasm();
  return Object.freeze(new PreparedReadonlyFcConnection());
}
