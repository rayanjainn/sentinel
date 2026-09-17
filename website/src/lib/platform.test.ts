import assert from "node:assert/strict";
import { describe, test } from "node:test";
import { detectOs, detectPlatform, type NavigatorLike } from "./platform.ts";

const UA = {
  macSafari:
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Safari/605.1.15",
  windowsChrome:
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36",
  linuxFirefox: "Mozilla/5.0 (X11; Linux x86_64; rv:142.0) Gecko/20100101 Firefox/142.0",
  linuxArm: "Mozilla/5.0 (X11; Linux aarch64; rv:142.0) Gecko/20100101 Firefox/142.0",
  android:
    "Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36",
  iphone:
    "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1",
  chromebook:
    "Mozilla/5.0 (X11; CrOS x86_64 16181.61.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36",
};

describe("detectOs", () => {
  test("prefers the client hints platform", () => {
    assert.equal(detectOs("macOS", UA.windowsChrome), "macos");
    assert.equal(detectOs("Windows", ""), "windows");
    assert.equal(detectOs("Linux", ""), "linux");
  });

  test("treats Android, Chrome OS and iOS hints as unsupported", () => {
    assert.equal(detectOs("Android", UA.android), null);
    assert.equal(detectOs("Chrome OS", UA.chromebook), null);
  });

  test("falls back to the user agent string", () => {
    assert.equal(detectOs("", UA.macSafari), "macos");
    assert.equal(detectOs("", UA.windowsChrome), "windows");
    assert.equal(detectOs("", UA.linuxFirefox), "linux");
  });

  test("does not mistake phones, Chromebooks or iPads for desktops", () => {
    assert.equal(detectOs("", UA.android), null);
    assert.equal(detectOs("", UA.iphone), null);
    assert.equal(detectOs("", UA.chromebook), null);
    // iPadOS Safari sends a Mac user agent but reports touch points.
    assert.equal(detectOs("", UA.macSafari, 5), null);
  });
});

describe("detectPlatform", () => {
  function chromium(platform: string, architecture: string): NavigatorLike {
    return {
      userAgent: UA.windowsChrome,
      maxTouchPoints: 0,
      userAgentData: {
        platform,
        getHighEntropyValues: async () => ({ architecture }),
      },
    };
  }

  test("reads the architecture from client hints", async () => {
    assert.deepEqual(await detectPlatform(chromium("macOS", "arm")), { os: "macos", arch: "arm64" });
    assert.deepEqual(await detectPlatform(chromium("macOS", "x86")), { os: "macos", arch: "x64" });
  });

  test("leaves the architecture unknown when hints are refused", async () => {
    const nav: NavigatorLike = {
      userAgent: UA.macSafari,
      userAgentData: {
        platform: "macOS",
        getHighEntropyValues: () => Promise.reject(new Error("denied")),
      },
    };
    assert.deepEqual(await detectPlatform(nav), { os: "macos", arch: null });
  });

  test("Safari on a Mac has no architecture hint", async () => {
    assert.deepEqual(await detectPlatform({ userAgent: UA.macSafari, maxTouchPoints: 0 }), {
      os: "macos",
      arch: null,
    });
  });

  test("spots Arm Linux from the user agent", async () => {
    assert.deepEqual(await detectPlatform({ userAgent: UA.linuxArm }), { os: "linux", arch: "arm64" });
  });
});
