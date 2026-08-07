# Python service acceptance fixture

This deliberately small external-project fixture has a different shape from the
real static-site workload used by the Host operations acceptance test. It uses
only Python's standard library, has no Plurora manifest, and starts without a
Dockerfile. The acceptance workflow must add that deployment description through
an approved Host ChangeSet before the service can be verified and deployed.
