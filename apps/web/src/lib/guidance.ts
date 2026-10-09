/** Hide `[[point: …]]` guidance markers (and a half-streamed `[[…` at the end) from display. */
export function stripMarkers(text: string): string {
  return text
    .replace(/\s?\[\[[^\]]*\]\]/g, "")
    .replace(/\s?\[\[[^\]]*$/, "")
    .replace(/\s?\[$/, "");
}
