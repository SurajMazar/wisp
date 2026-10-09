/** Bucket a timestamp the way chat apps group history. */
export function dayGroup(ms: number, now = new Date()): "Today" | "Yesterday" | "Previous 7 days" | "Older" {
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  if (ms >= startOfToday) return "Today";
  if (ms >= startOfToday - 86_400_000) return "Yesterday";
  if (ms >= startOfToday - 7 * 86_400_000) return "Previous 7 days";
  return "Older";
}
