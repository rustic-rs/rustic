#!/bin/bash
PROJECT_VERSION=$(git describe --tags) cargo build -r $@
