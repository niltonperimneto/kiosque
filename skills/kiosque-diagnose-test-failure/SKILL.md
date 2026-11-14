---
name: kiosque-diagnose-test-failure
description: Reproduce and diagnose Kiosque test, native build or package smoke failures and add focused regression coverage.
---

# kiosque-diagnose-test-failure

Read [testing](../../docs/testing.md). Preserve the first failing command, output, feature selection and environment versions. Reproduce the smallest applicable suite; distinguish missing tools or dependencies from an assertion failure. Trace the real service/adapter boundary before changing a mock. Add a regression asserting observable behavior, run the affected suite, and expand checks only for newly affected boundaries. Do not hide flakes by repeated retries or weaken assertions to make a test pass. Report remaining evidence gaps.
