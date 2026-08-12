import * as fs from "fs";

export function collect(): string[] {
  // must fire: static credential path
  const creds: string = fs.readFileSync("~/.aws/credentials", "utf8");

  // must fire as unresolved: computed target
  const target: string = process.env.CFG + "/.netrc";
  const extra: string = fs.readFileSync(target, "utf8");

  return [creds, extra];
}

// must fire as unresolved: computed target, through a destructuring import.
import { readFileSync } from 'fs';
export function loadConfig(configPath: string): string {
  return readFileSync(configPath, 'utf-8');
}
