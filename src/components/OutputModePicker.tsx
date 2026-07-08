import { OUTPUT_MODES, type OutputMode } from "../lib/types";

interface Props {
  value: OutputMode;
  onChange: (mode: OutputMode) => void;
  disabled?: boolean;
}

/** Segmented picker for the output rewrite mode. */
export function OutputModePicker({ value, onChange, disabled }: Props) {
  const active = OUTPUT_MODES.find((m) => m.value === value);
  return (
    <div className="mode-picker">
      <div className="mode-picker__row" role="radiogroup" aria-label="Output mode">
        {OUTPUT_MODES.map((mode) => (
          <button
            key={mode.value}
            type="button"
            role="radio"
            aria-checked={mode.value === value}
            disabled={disabled}
            className={
              "mode-picker__option" +
              (mode.value === value ? " mode-picker__option--active" : "")
            }
            onClick={() => onChange(mode.value)}
          >
            {mode.label}
          </button>
        ))}
      </div>
      {active && <p className="mode-picker__hint">{active.description}</p>}
    </div>
  );
}
