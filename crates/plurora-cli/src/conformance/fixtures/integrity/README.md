Integrity conformance fixtures are generated dynamically with `sequoia-openpgp`
during test setup. This avoids checking private test keys into the repository
while still exercising valid signatures, wrong-key failures, corrupt signatures,
and public-key fingerprint extraction.
