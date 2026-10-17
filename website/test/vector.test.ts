/**
 * The interop vector djmanzo's own tests check too
 * (`crates/dj-app/src/account.rs`, `the_website_s_tokens_verify`): one test
 * key, one licence and one activation, signed by the Worker's own code.
 * Ed25519 is deterministic, so if either side changes the format, one of
 * the two tests fails.
 */
import { describe, expect, it } from "vitest";
import { sign } from "../worker/licence";
import { testEnv } from "./kv";

// A key made for this test and nothing else.
export const TEST_PRIVATE = "MC4CAQAwBQYDK2VwBCIEIKcArghTIF8IBZwy9eWgVBatMTjVcikdZa1ba5fVlsnn";
export const TEST_PUBLIC = "b1LFMiWzX6DnI2eOjRPiFCEsOwlR0sSX3KdCefRzV9I";

const LICENCE = { v: 1, lid: "l_test", uid: "u_test", name: "DJ Vector", email: null, edition: "karaoke", plan: "yearly", major: 1, seats: 3, iat: 1800000000, exp: 1831536000 };
const ACTIVATION = { v: 1, lid: "l_test", machine: "machine-vector-0123456789", edition: "karaoke", plan: "yearly", major: 1, iat: 1800000000, exp: 1802592000 };

export const LICENCE_TOKEN =
  "DJM1.eyJ2IjoxLCJsaWQiOiJsX3Rlc3QiLCJ1aWQiOiJ1X3Rlc3QiLCJuYW1lIjoiREogVmVjdG9yIiwiZW1haWwiOm51bGwsImVkaXRpb24iOiJrYXJhb2tlIiwicGxhbiI6InllYXJseSIsIm1ham9yIjoxLCJzZWF0cyI6MywiaWF0IjoxODAwMDAwMDAwLCJleHAiOjE4MzE1MzYwMDB9.qSjSSkfYf1QUDrY4hbOwSNDNxbdpIxnPxLnkWxBjo62mdVBvPKmS7zYuOqQok5Pc-pXK4DOJplF2ayschu_pCQ";
export const ACTIVATION_TOKEN =
  "DJA1.eyJ2IjoxLCJsaWQiOiJsX3Rlc3QiLCJtYWNoaW5lIjoibWFjaGluZS12ZWN0b3ItMDEyMzQ1Njc4OSIsImVkaXRpb24iOiJrYXJhb2tlIiwicGxhbiI6InllYXJseSIsIm1ham9yIjoxLCJpYXQiOjE4MDAwMDAwMDAsImV4cCI6MTgwMjU5MjAwMH0.FPcpbeH_UqICfat_MLesZHvvlsDOLoPJNZSxC3uSfL2fd74bHMQfpKhAXgp-8MFpAlqFtpSAd9HUFm3s1s6-CA";

describe("the interop vector", () => {
  it("signs the same bytes every time", async () => {
    const env = testEnv({ LICENCE_PRIVATE_KEY: TEST_PRIVATE });
    const licence = await sign(env, "DJM1", LICENCE);
    const activation = await sign(env, "DJA1", ACTIVATION);
    expect(licence).toBe(LICENCE_TOKEN);
    expect(activation).toBe(ACTIVATION_TOKEN);
  });
});
