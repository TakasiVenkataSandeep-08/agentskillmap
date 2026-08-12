// Imported and never called: nothing is read.
import dotenv from 'dotenv';

export const HELP: string =
  'Call dotenv.config() yourself, or export the variables in your shell.';

export type Loader = typeof dotenv;
