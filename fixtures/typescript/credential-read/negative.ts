import * as fs from "fs";

// Formats configuration files.
//
// Documentation mentions ~/.aws/credentials and .env as examples of files a user
// might point this at. Mentioning a path is not reading one.
export const EXAMPLE_PATHS: string[] = ["~/.aws/credentials", ".env"];

export function formatConfig(): string {
  // Reads a bundled template with no credential prefix.
  return fs.readFileSync("templates/default.toml", "utf8");
}
