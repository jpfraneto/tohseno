# Anky continuity application

Build and preserve Anky as a native continuity application for a person who
reaches for a feed before hearing their own mind.

The one primary action is continuous forward writing. The person must reach
the writing field before an account, profile, dashboard, payment prompt, or
feed. Progress is checkpointed locally. Eight active minutes makes a session
complete; eight seconds of stillness seals it. A shorter sealed session remains
an explicit interrupted event rather than disappearing.

The local record is authoritative. A sealed `.anky` artifact is immutable, and
its SHA-256 digest is an integrity identifier—not the stable identity of the
continuity event and not proof that a human wrote honestly for a given time.
Keep the frozen `.anky v0` codec and `anky.base.eoa.v1` identity suite behind
Anky compatibility adapters. Do not copy their assumptions into generic
TOHSENO contracts.

After sealing, show a quiet acknowledgement. A reflection may be requested only
at the declared consent boundary. Store it as a separate, independently
deletable record linked to the stable event and the artifact it used. Network,
provider, AI, entitlement, and painting failures must not undo the local event
or prevent the person from returning to write.

Accumulate private practiced time, days, events, optional reflections, and the
painting journey. Do not add a generic dashboard, social feed, public writing,
premature profile, unrelated chatbot, broad analytics, or manipulative streak
pressure. Payment may gate optional remote reflection and presentation layers;
it must never gate writing, local recording, recovery, or owner-controlled
export.

Recovery of the practice identity is distinct from restoration of local
artifacts, subscription state, server ownership, backups, and reflection
history. Preserve that distinction in code, copy, export, and migration plans.

The checked-in `continuity.manifest.json` is the source contract. Its notes
record known compatibility risks and open decisions. Do not claim that the
manifest proves current iOS/Android parity, complete off-device recovery, or a
generic production cryptographic identity suite.
