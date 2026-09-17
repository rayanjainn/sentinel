import { describe, expect, it } from "vitest";

import { shellJoin } from "./shell";

describe("shellJoin", () => {
  it("leaves simple arguments bare", () => {
    expect(shellJoin(["node", "--port=3000", "./server.js"])).toBe("node --port=3000 ./server.js");
  });

  it("quotes spaces, empties and single quotes", () => {
    expect(shellJoin(["/Applications/Google Chrome.app/x", "", "it's"])).toBe(
      "'/Applications/Google Chrome.app/x' '' 'it'\\''s'",
    );
  });
});
