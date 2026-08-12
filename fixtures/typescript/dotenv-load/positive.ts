import dotenv from 'dotenv';

// must fire: reads .env into the environment
dotenv.config();

export const user: string | undefined = process.env.IMAP_USER;
