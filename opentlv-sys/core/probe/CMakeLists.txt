# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Marek Cingel

# Records the C toolchain a Verified Source build would use, so that
# core/verified_core.py accepts only an equivalent Verified Binary.
cmake_minimum_required(VERSION 3.16)
project(opentlv_core_probe C)

file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/core-probe.json"
    "{\"COMPILER_ID\":\"${CMAKE_C_COMPILER_ID}\","
    "\"COMPILER_VERSION\":\"${CMAKE_C_COMPILER_VERSION}\","
    "\"SIZEOF_DATA_PTR\":\"${CMAKE_SIZEOF_VOID_P}\","
    "\"SYSTEM\":\"${CMAKE_SYSTEM_NAME}\","
    "\"PROCESSOR\":\"${CMAKE_SYSTEM_PROCESSOR}\","
    "\"COMPILER_ARCHITECTURE\":\"${CMAKE_C_COMPILER_ARCHITECTURE_ID}\","
    "\"CROSS_COMPILING\":\"${CMAKE_CROSSCOMPILING}\"}\n")
