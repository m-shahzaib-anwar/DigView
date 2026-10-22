/** Approved DigView identity: muted green document and open teal search lens. */
export function BrandIcon() {
  return (
    <img
      className="digview-icon"
      src="/digview.svg"
      alt=""
      aria-hidden="true"
    />
  );
}

export function BrandWordmark() {
  return (
    <span className="digview-wordmark" aria-label="DigView">
      <BrandIcon />
      <span aria-hidden="true">
        <strong>Dig</strong>View
      </span>
    </span>
  );
}
