"""Verify DKIM signatures produced by the platform with an independent
implementation (dkimpy). Interoperability check for services/dkim.rs.

    pip install dkimpy
    DKIM_DUMP_DIR=/tmp/dkim cargo test --lib signs_common_message_shapes
    python tests/verify_dkim.py /tmp/dkim
"""
import os, sys, dkim
d = sys.argv[1]
pub = open(os.path.join(d, 'pubkey.txt')).read().strip()
dnsfunc = lambda name, timeout=5: f"v=DKIM1; k=rsa; p={pub}".encode()
ok_all = True
for name in ['text', 'alternative', 'mixed']:
    raw = open(os.path.join(d, name + '.eml'), 'rb').read()
    ok = dkim.DKIM(raw).verify(dnsfunc=dnsfunc)
    ok_all &= ok
    print(f"{name:12s} {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok_all else 1)
