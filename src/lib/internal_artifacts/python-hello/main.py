#!/usr/bin/env python3
"""
Python Hello World Application
This is a simple demo artifact that demonstrates Python execution in HandBox
"""

import sys
import os
from datetime import datetime

def main():
    print("🐍 Python Artifact Demo")
    print("=" * 40)
    print()
    print("Hello from HandBox Artifact System!")
    print()
    print(f"Python version: {sys.version}")
    print(f"Current time: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"Current directory: {os.getcwd()}")
    print()

    if len(sys.argv) > 1:
        print("Arguments received:")
        for i, arg in enumerate(sys.argv[1:], 1):
            print(f"  {i}. {arg}")
    else:
        print("No arguments provided")

    print()
    print("✅ Python artifact executed successfully!")

if __name__ == "__main__":
    main()
