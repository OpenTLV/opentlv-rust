// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

#include <stdint.h>
#include <stdio.h>

size_t opentlv_test_fixed_abi(size_t index);

int main(void) {
    for (size_t i = 0;; ++i) {
        size_t value = opentlv_test_fixed_abi(i);
        if (value == SIZE_MAX) break;
        printf("%zu\n", value);
    }
    return 0;
}
