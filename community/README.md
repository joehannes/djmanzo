# The community index of packs

`packs.json` is the index djmanzo reads when a DJ presses *Browse packs…* in
the Singers panel, until they take it out of their list. It names packs —
looks and the MilkDrop presets they draw — and where each is. djmanzo does
not host them: each lives wherever its maker puts it.

## Adding your pack

1. Make it in djmanzo: *Packs — Make a pack of your presets…*. It is signed
   with your own key; the form shows that key's fingerprint.
2. Put the `.djpack` where an `https` address serves it — a release of your
   own repository, say.
3. Open a pull request adding one entry to `packs`:

   ```json
   {
     "name": "Booth Lights",
     "author": "DJ Manzo",
     "licence": "CC BY-SA 4.0",
     "version": "1.0.0",
     "description": "Two late-night presets from the booth.",
     "url": "https://example.org/booth-lights.djpack",
     "key": "2622 cdeb 7fc8 f353"
   }
   ```

`name` and `key` must be the pack's own: djmanzo refuses a pack whose name or
signing key is not the one listed here. A new version is the same entry with
a new `version` and `url`, signed with the same key.

Only presets that are yours to share under the licence you name. The test
`the_community_index_is_readable` checks that every entry here can be shown.
