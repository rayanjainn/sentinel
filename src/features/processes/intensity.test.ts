import { describe, expect, it } from "vitest";

import { quantile, relativeScale } from "./intensity";

describe("quantile", () => {
  it("interpolates", () => {
    expect(quantile([0, 10], 0.5)).toBe(5);
    expect(quantile([], 0.9)).toBe(0);
  });
});

describe("relativeScale", () => {
  it("tints only values that stand out from the distribution", () => {
    const values = Array.from({ length: 100 }, (_, i) => i);
    const score = relativeScale(values, 0);
    expect(score(50)).toBe(0);
    expect(score(94)).toBe(0);
    expect(score(99)).toBe(1);
    expect(score(97)).toBeGreaterThan(0.2);
    expect(score(97)).toBeLessThan(1);
  });

  it("does not tint a machine where everything is below the noise floor", () => {
    const score = relativeScale([0.1, 0.3, 0.8, 1.2], 2);
    expect(score(1.2)).toBe(0);
  });

  it("adapts to the current distribution rather than a fixed number", () => {
    const busy = relativeScale([...Array(50).fill(40), 90], 2);
    const quiet = relativeScale([...Array(50).fill(1), 30], 2);
    expect(busy(40)).toBe(0);
    expect(quiet(30)).toBe(1);
  });

  it("gives a single hog the full tint", () => {
    const score = relativeScale([...Array(200).fill(0.5), 350], 2);
    expect(score(350)).toBe(1);
    expect(score(0.5)).toBe(0);
  });
});
