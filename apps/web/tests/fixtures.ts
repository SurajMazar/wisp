import { AppSettings } from "@wisp/contracts";

/** Default settings straight from the contract, so fixtures never drift from Rust's defaults. */
export const defaultSettings = (): AppSettings => AppSettings.parse({});
