/** Short in-app privacy note. Reinforces the local-only model and the policy
 * that confidential work must use approved enterprise endpoints. */
export function PrivacyNote() {
  return (
    <div className="privacy-note" role="note">
      <span className="privacy-note__icon" aria-hidden>
        🔒
      </span>
      <p className="privacy-note__text">
        Everything runs locally — audio is captured to a temp file, transcribed
        and rewritten on this machine, then the temp file is deleted. No
        history, telemetry, or cloud calls. For confidential or regulated work,
        only use approved enterprise endpoints and follow your organization's
        data-handling policies.
      </p>
    </div>
  );
}
