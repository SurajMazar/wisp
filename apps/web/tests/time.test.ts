import { describe, expect, it } from "vitest";
import { dayGroup } from "../src/lib/time";

describe("dayGroup", () => {
  const now = new Date(2026, 9, 9, 15, 0);
  const at = (d: number, h = 10) => new Date(2026, 9, d, h).getTime();
  it("buckets history like other chat apps", () => {
    expect(dayGroup(at(9, 1), now)).toBe("Today");
    expect(dayGroup(at(8, 23), now)).toBe("Yesterday");
    expect(dayGroup(at(3), now)).toBe("Previous 7 days");
    expect(dayGroup(at(1), now)).toBe("Older");
  });
});
