// Read the product version, not the independent website package version.
import cargo from "../../../Cargo.toml?raw";
const match = cargo.match(/^version\s*=\s*"([^"]+)"/m);
if (!match) throw new Error("Cargo.toml package version is missing");
export const version = match[1];
