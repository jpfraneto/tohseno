export interface RelayEnvelope {
  schema: "tohseno.companion-envelope/1";
  envelope_id: string;
  mailbox_id: string;
  sender_device_id: string;
  recipient_device_id: string;
  sender_sequence: number;
  created_at: string;
  expires_at: string;
  ephemeral_public_key: string;
  nonce: string;
  ciphertext: string;
  signature: string;
}

export interface PushRegistration {
  schema: "tohseno.companion-push-registration/1";
  mailboxId: string;
  deviceId: string;
  token: string;
  registeredAt: number;
}

export interface MailboxEvent {
  kind: "envelope" | "revoked";
  cursor: number;
  envelope?: RelayEnvelope;
}

export interface RelayMetrics {
  pairingSessions: number;
  mailboxes: number;
  revokedMailboxes: number;
  envelopes: number;
  bytes: number;
  pushRegistrations: number;
  liveSubscribers: number;
}
