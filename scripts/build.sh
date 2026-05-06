#!/usr/bin/env bash
PROJECT_VERSION=$(git describe --tags) cargo build -r "$@"
