# lnd-nwc
A NWC wallet service that runs on your Lightning node and implements NIP-47

# TODO
- [ ] Find a nostr library (most probably https://rust-nostr.org)
- [ ] Connection URI management (creation, storage, removal)
- [ ] Implement event support
- [ ] Deamon support (start, stop) and document autostart on boot
- [ ] Create a QR code of the Connection URI


# Nostr Wallet Connect URI (https://nostr-nips.com/nip-47)

Format: `nostr+walletconnect:<PUBKEY>?secret=<SECRET>&relay=<RELAY_URL>`
* `PUBKEY`: hex-encoded pubkey
* `SECRET`: 32-byte randomly generated hex encoded string
* `RELAY_URL`: URL of the relay to use (may be more than one)

The client uses `SECRET` to sign its messages
