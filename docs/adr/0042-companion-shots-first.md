# ADR 0042: Companion opens directly to your Shots

Status: accepted

Date: 2026-09-04

The owner rejected the pocket workshop diagram during physical use: multiple
connection descriptions, actors, thresholds and repeated navigation obscured
the apps themselves.

Companion now opens to one list of active apps from the paired Mac workspace.
Generated Shots and adopted projects share that list, including projects
materialized through verified network installation or Fork. Apps created by an
external agent appear once connected to that workspace; arbitrary phone apps
or unconnected source directories are not discovered implicitly. A Claim by
itself is not evidence of a locally prepared or installed app.

A floating bottom bar contains Shots, a central Tohseno-logo Take a Shot button,
and Discover. Updates and Profile remain available through a small top menu.
Missing synchronization is distinct from a verified empty workspace. An
explicit refresh requests the existing signed full workspace snapshot.

This supersedes ADR 0039's Companion home composition only. The Mac shell,
existing factory, authority, six-state model, pairing, encrypted synchronization,
source verification, Ship/Update/Claim and intended-device rules are retained.
Nearby Workshop Session status is not a prerequisite for displaying the app
list or submitting existing durable commands. No public release is activated.
