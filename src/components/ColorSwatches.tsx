import { COLOR_OPTIONS } from "../classificationLabels";

interface ColorSwatchesProps {
  value: string[];
  onChange: (value: string[]) => void;
  ariaLabel: string;
}

export function ColorSwatches({ value, onChange, ariaLabel }: ColorSwatchesProps) {
  const toggleColor = (color: string) => {
    onChange(value.includes(color) ? value.filter((item) => item !== color) : [...value, color]);
  };

  return (
    <div className="classification-color-picker" role="group" aria-label={ariaLabel}>
      {COLOR_OPTIONS.map(([color, label]) => (
        <button
          type="button"
          key={color}
          className={
            value.includes(color)
              ? "classification-color-choice is-active"
              : "classification-color-choice"
          }
          aria-label={label}
          aria-pressed={value.includes(color)}
          title={label}
          onClick={() => toggleColor(color)}
        >
          <i data-color={color} aria-hidden="true" />
          <span className="sr-only">{label}</span>
        </button>
      ))}
    </div>
  );
}
