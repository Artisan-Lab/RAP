#!/bin/bash

set -e

cargo install --path rapx

cargo rapx -help
