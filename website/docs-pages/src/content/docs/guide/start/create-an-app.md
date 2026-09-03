---
title: Create an app
description: Turn one concrete intention and optional visual references into a local native iPhone app.
---

Creation is the secondary path for a person who does not already have an Xcode project.

## Write the intention

Use the persistent **One Shot** dock. Describe one small loop:

> Make a breathing timer. One large circle expands for four seconds and contracts for six. Tapping anywhere starts or pauses. Keep the last duration on the phone.

A useful intention states:

- the person and moment;
- the smallest complete action loop;
- what must remain on the device;
- what “done” looks like.

Avoid broad requests such as “make a wellness app.” The factory can implement a bounded object more honestly than a product category.

## Add references only when they carry information

You may pick or drop up to eight PNG or JPEG images. Tohseno copies and validates the exact bytes before execution. Use references for layout, visual tone, or the current incorrect state. Do not put passwords, credentials, private keys, provisioning profiles, or unrelated personal images in a reference.

## Name or let the implementation choose

An app name is optional. If supplied, it is authoritative. If omitted, the service first reserves a collision-safe technical slug, then the one existing implementation pass chooses a concise user-facing name from the intention. There is no separate naming or planning model call.

## Send once

Plain **Return** sends from the focused composer. **Shift–Return** inserts a line. The client uses exactly-once submission guards, and the service persists the exact request before execution. Repeating the same submission must not create a second command.

## What happens next

The engine stages the required Apple Fascia and resource placeholders, composes and accepts the Shot-specific Genome itself, then invokes one implementation harness. There is no Conception phase or `.tohseno/CONCEPTION.md`. Source is not completion: deterministic Xcode, signing, device, installation, and launch gates still follow.

If the phone is unavailable after a verified build, the app becomes **Ready to install** and resumes delivery when the phone returns. It does not rerun the coding agent merely because the cable was absent.

Next: [evolve an app](/guide/start/evolve-an-app/).
