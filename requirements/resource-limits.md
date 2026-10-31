# Resource limits

The user approved a 1 MiB maximum response body for the Jellyfin server
connectivity probe on 2026-10-31. This applies only to `/System/Info/Public`,
not media, library requests, or artwork. Exceeding the limit cancels the probe
and reports a failed connectivity check.
