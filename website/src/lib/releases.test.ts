import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  ASSET_SPECS,
  assetFileName,
  formatBytes,
  lookupLatestRelease,
  parseRelease,
} from "./releases.ts";

function githubRelease(version: string, names: string[]) {
  return {
    tag_name: `v${version}`,
    html_url: `https://github.com/rayanjainn/sentinel/releases/tag/v${version}`,
    published_at: "2026-09-14T10:00:00Z",
    assets: names.map((name, index) => ({
      name,
      size: 10_000_000 + index,
      browser_download_url: `https://github.com/rayanjainn/sentinel/releases/download/v${version}/${name}`,
    })),
  };
}

const ALL_NAMES = [
  "Sentinel-0.2.0-macos-arm64.dmg",
  "Sentinel-0.2.0-macos-x64.dmg",
  "Sentinel-0.2.0-macos-arm64.app.tar.gz",
  "Sentinel-0.2.0-windows-x64-setup.exe",
  "Sentinel-0.2.0-windows-x64.msi",
  "Sentinel-0.2.0-linux-x64.AppImage",
  "Sentinel-0.2.0-linux-x64.deb",
  "SHA256SUMS.txt",
];

describe("asset names", () => {
  test("match the release workflow's naming scheme", () => {
    assert.deepEqual(
      ASSET_SPECS.map((spec) => assetFileName("0.1.0", spec)),
      [
        "Sentinel-0.1.0-macos-arm64.dmg",
        "Sentinel-0.1.0-macos-x64.dmg",
        "Sentinel-0.1.0-windows-x64-setup.exe",
        "Sentinel-0.1.0-windows-x64.msi",
        "Sentinel-0.1.0-linux-x64.AppImage",
        "Sentinel-0.1.0-linux-x64.deb",
      ],
    );
  });
});

describe("parseRelease", () => {
  test("maps every expected asset and the checksums file", () => {
    const release = parseRelease(githubRelease("0.2.0", ALL_NAMES));
    assert.ok(release);
    assert.equal(release.version, "0.2.0");
    assert.equal(release.tag, "v0.2.0");
    assert.equal(Object.keys(release.assets).length, ASSET_SPECS.length);
    assert.equal(
      release.assets["macos-arm64"]?.url,
      "https://github.com/rayanjainn/sentinel/releases/download/v0.2.0/Sentinel-0.2.0-macos-arm64.dmg",
    );
    assert.equal(release.checksums?.name, "SHA256SUMS.txt");
  });

  test("ignores files from a different version and reports missing ones", () => {
    const release = parseRelease(
      githubRelease("0.2.0", ["Sentinel-0.1.0-macos-arm64.dmg", "Sentinel-0.2.0-linux-x64.deb"]),
    );
    assert.ok(release);
    assert.equal(release.assets["macos-arm64"], undefined);
    assert.ok(release.assets["linux-x64-deb"]);
    assert.equal(release.checksums, null);
  });

  test("rejects responses without a tag", () => {
    assert.equal(parseRelease({ message: "Not Found" }), null);
    assert.equal(parseRelease(null), null);
  });
});

describe("lookupLatestRelease", () => {
  const respond = (status: number, body: unknown) => async () =>
    new Response(JSON.stringify(body), { status });

  test("returns the parsed release", async () => {
    const result = await lookupLatestRelease(respond(200, githubRelease("0.2.0", ALL_NAMES)));
    assert.equal(result.status, "found");
  });

  test("treats 404 as no release yet", async () => {
    assert.deepEqual(await lookupLatestRelease(respond(404, { message: "Not Found" })), {
      status: "none",
    });
  });

  test("explains rate limiting", async () => {
    const result = await lookupLatestRelease(respond(403, { message: "API rate limit exceeded" }));
    assert.equal(result.status, "error");
    assert.match(result.status === "error" ? result.reason : "", /limit/);
  });

  test("survives network failures", async () => {
    const result = await lookupLatestRelease(async () => {
      throw new TypeError("Failed to fetch");
    });
    assert.deepEqual(result, { status: "error", reason: "GitHub could not be reached" });
  });
});

describe("formatBytes", () => {
  test("uses megabytes with sensible precision", () => {
    assert.equal(formatBytes(0), "");
    assert.equal(formatBytes(12_345_678), "12.3 MB");
    assert.equal(formatBytes(148_000_000), "148 MB");
  });
});
