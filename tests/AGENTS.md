# Tests

- Assert behavior and failure recovery, not source wording. Use synthetic fixtures without tokens or user data.
- Unit/service tests use local HTTP, fake commands, temporary storage and explicit time. No live API calls.
- System suites run only in disposable VMs. Assert both user and system installation paths are private before mutation.
- Bound asynchronous waits and clean up subprocesses on success, failure and cancellation. Preserve the first failure and its logs.
- Shared runner and CI commands are documented in [testing](../docs/testing.md).
