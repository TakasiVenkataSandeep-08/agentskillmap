"""Generates a video via a hosted API."""

import os

from dotenv import load_dotenv

# must fire: reads .env into the environment
load_dotenv()

API_KEY = os.getenv("OPENAI_API_KEY")
