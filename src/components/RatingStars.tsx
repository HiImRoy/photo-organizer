import { useState, type FocusEvent } from "react";

type RatingStarsProps = {
  value: number;
  onChange: (rating: number) => void;
  className: string;
  ariaLabel: string;
  buttonLabel: (rating: number) => string;
  pressedMode?: "threshold" | "exact";
  activeCharacter?: string;
  inactiveCharacter?: string;
};

export function RatingStars({
  value,
  onChange,
  className,
  ariaLabel,
  buttonLabel,
  pressedMode = "threshold",
  activeCharacter = "★",
  inactiveCharacter = "☆",
}: RatingStarsProps) {
  const [hoveredRating, setHoveredRating] = useState<number | null>(null);
  const previewRating = hoveredRating ?? value;

  const clearHover = () => setHoveredRating(null);
  const handleBlur = (event: FocusEvent<HTMLDivElement>) => {
    const nextTarget = event.relatedTarget;
    if (!(nextTarget instanceof Node) || !event.currentTarget.contains(nextTarget)) {
      clearHover();
    }
  };

  return (
    <div
      className={className}
      role="group"
      aria-label={ariaLabel}
      onMouseLeave={clearHover}
      onBlur={handleBlur}
    >
      {Array.from({ length: 5 }, (_, index) => {
        const rating = index + 1;
        const isPreviewActive = rating <= previewRating;
        const isPressed = pressedMode === "exact" ? value === rating : rating <= value;
        const label = buttonLabel(rating);
        return (
          <button
            type="button"
            className={isPreviewActive ? "is-active" : ""}
            key={rating}
            aria-label={label}
            aria-pressed={isPressed}
            title={label}
            onMouseEnter={() => setHoveredRating(rating)}
            onFocus={() => setHoveredRating(rating)}
            onClick={() => onChange(rating)}
          >
            {isPreviewActive ? activeCharacter : inactiveCharacter}
          </button>
        );
      })}
    </div>
  );
}
