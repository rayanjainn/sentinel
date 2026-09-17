import { describe, expect, it } from "vitest";

import { formatEndpoint } from "./net";

describe("formatEndpoint", () => {
  it("formats v4, v6 and wildcard endpoints", () => {
    expect(formatEndpoint("192.168.1.4", 443, "v4")).toBe("192.168.1.4:443");
    expect(formatEndpoint("2606:4700::6810", 443, "v6")).toBe("[2606:4700::6810]:443");
    expect(formatEndpoint("0.0.0.0", 5353, "v4")).toBe("*:5353");
    expect(formatEndpoint("::", 22, "v6")).toBe("*:22");
    expect(formatEndpoint(null, null, "v4")).toBe("*");
  });
});
