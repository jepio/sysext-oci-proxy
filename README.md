# Introduction
This is a proxy server that allows systemd-sysupdate to pull system extensions
(sysexts) from an OCI compliant registry.

Image tags need to match the `MatchPattern=` value in the sysupdate config
file.

Limitations:
- this relies on a local registry being accessible at `localhost:5000`
- no nested container image names
- sysexts need to be disk images (raw) files, pushed using oras

# Example
Config `/etc/sysupdate.kubernetes.d/oci.conf`:
```ini
[Transfer]
Verify=false

[Source]
Type=url-file
Path=http://localhost:5001/kubernetes
MatchPattern=kubernetes-@v-%a.raw

[Target]
InstancesMax=3
Type=regular-file
Path=/opt/extensions/kubernetes
CurrentSymlink=/etc/systemd/extensions/kubernetes.raw
```

Push sysext:
```shell
$ oras push --insecure localhost:5000/kubernetes:v1.27.8-x86-64 kubernetes-v1.27.6-x86-64.raw
```

List updates:
```shell
$ systemd-sysupdate -C kubernetes list
Discovering installed instances…
Discovering available instances…
⤵️ Acquiring manifest file http://localhost:5001/kubernetes/SHA256SUMS…
Pulling 'http://localhost:5001/kubernetes/SHA256SUMS'.
Downloading 288B for http://localhost:5001/kubernetes/SHA256SUMS.
Acquired 288B.
Download of http://localhost:5001/kubernetes/SHA256SUMS complete.
Operation completed successfully.
Exiting.
Determining installed update sets…
Determining available update sets…
  VERSION INSTALLED AVAILABLE ASSESSMENT
↻ v1.27.8               ✓     candidate
● v1.27.7     ✓         ✓     current
  v1.27.6               ✓     available
```

Pull update:
```shell
$ systemd-sysupdate -C kubernetes update
Discovering installed instances…
Discovering available instances…
⤵️ Acquiring manifest file http://localhost:5001/kubernetes/SHA256SUMS…
Pulling 'http://localhost:5001/kubernetes/SHA256SUMS'.
Downloading 288B for http://localhost:5001/kubernetes/SHA256SUMS.
Acquired 288B.
Download of http://localhost:5001/kubernetes/SHA256SUMS complete.
Operation completed successfully.
Exiting.
Determining installed update sets…
Determining available update sets…
Selected update 'v1.27.8' for install.
Making room for 1 updates…
Removed no instances.
⤵️ Acquiring http://localhost:5001/kubernetes/kubernetes-v1.27.8-x86-64.raw  /opt/extensions/kubernetes/kubernetes-v1.27.8-x86-64.raw...
Pulling 'http://localhost:5001/kubernetes/kubernetes-v1.27.8-x86-64.raw', saving as '/opt/extensions/kubernetes/.#sysupdatekubernetes-v1.27.8-x86-64.rawc9bfac3f01cf09b9'.
Downloading 99.8M for http://localhost:5001/kubernetes/kubernetes-v1.27.8-x86-64.raw.
Got 1% of http://localhost:5001/kubernetes/kubernetes-v1.27.8-x86-64.raw.
Acquired 99.8M.
Download of http://localhost:5001/kubernetes/kubernetes-v1.27.8-x86-64.raw complete.
Operation completed successfully.
Exiting.
Successfully acquired 'http://localhost:5001/kubernetes/kubernetes-v1.27.8-x86-64.raw'.
Successfully installed 'http://localhost:5001/kubernetes/kubernetes-v1.27.8-x86-64.raw' (url-file) as '/opt/extensions/kubernetes/kubernetes-v1.27.8-x86-64.raw' (regular-file).
Updated symlink '/opt/extensions/kubernetes/kubernetes-v1.27.7-x86-64.raw' → 'kubernetes-v1.27.8-x86-64.raw'.
✨ Successfully installed update 'v1.27.8'.
```
