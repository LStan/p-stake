<h1 align="center">
  <code>p-stake</code>
</h1>
<p align="center">
  A pinocchio-based Stake program.
</p>

## Overview

This repository contains a **proof-of-concept** of a reimplementation of the [Stake program](https://github.com/solana-program/stake) using [`pinocchio`](https://github.com/anza-xyz/pinocchio). The purpose is to have an implementation that optimizes the compute units, while being fully compatible with the original implementation - i.e., support the exact same instruction and account layouts as StakeState, byte for byte.

## Features

- `no_std` crate
- Same instruction and account layout as StakeState
- Minimal CU usage

## Status

- [x] Instructions
- [x] Basic instruction tests
- [x] Existing Stake tests

## Building

```bash
cd program
make build
```

## Testing

```bash
cd program
make test
```
