"""Transcribe short Nintendo DS mic clips on a computer on the same LAN."""

import argparse
import ipaddress
import socketserver
import struct
import unicodedata

import numpy as np

RATE = 8000
MAX_BYTES = 5 * RATE * 2
MIN_BYTES = RATE // 2
MAX_TEXT = 144


def receive(connection, count):
    data = bytearray()
    while len(data) < count:
        chunk = connection.recv(count - len(data))
        if not chunk:
            raise EOFError("Incomplete clip")
        data.extend(chunk)
    return data


def speech_text(text):
    normalized = unicodedata.normalize("NFKD", text).encode("ascii", "ignore").decode("ascii")
    return " ".join(normalized.split())[:MAX_TEXT].strip()


def transcribe(model, pcm):
    samples = np.frombuffer(pcm, dtype="<i2").astype(np.float32) / 32768
    # The DS sends 8 kHz PCM. Whisper expects a normalized 16 kHz waveform.
    audio = np.empty(samples.size * 2, dtype=np.float32)
    audio[::2] = samples
    audio[1:-1:2] = (samples[:-1] + samples[1:]) * 0.5
    audio[-1] = samples[-1]
    segments, _ = model.transcribe(
        audio, language="en", beam_size=2, condition_on_previous_text=False, vad_filter=True,
        hotwords="innkeeper, room, soup, supplies, sacks",
    )
    return speech_text(" ".join(segment.text for segment in segments))


class Request(socketserver.BaseRequestHandler):
    def handle(self):
        client = self.request
        client.settimeout(8)
        try:
            magic, size = struct.unpack("<4sI", receive(client, 8))
            if magic != b"ARD1" or not MIN_BYTES <= size <= MAX_BYTES or size % 2:
                client.sendall(b"ERR INVALID AUDIO\n")
                return
            pcm = receive(client, size)
            if not any(pcm):
                client.sendall(b"ERR NO SPEECH HEARD\n")
                return
            text = transcribe(self.server.model, pcm)
            response = f"OK {text}" if text else "ERR NO SPEECH HEARD"
            client.sendall(response.encode("ascii") + b"\n")
        except (EOFError, OSError):
            return
        except Exception as exc:
            # Keep audio and transcripts out of logs, including on model errors.
            print(f"Transcription failed ({type(exc).__name__}).", flush=True)
            try:
                client.sendall(b"ERR COULD NOT TRANSCRIBE\n")
            except OSError:
                pass


class Companion(socketserver.TCPServer):
    allow_reuse_address = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bind", required=True, type=ipaddress.IPv4Address, help="This computer's LAN IPv4 address")
    parser.add_argument("--model", default="base.en", help="Faster Whisper model name or local directory")
    parser.add_argument("--port", default=8765, type=int)
    args = parser.parse_args()
    if not args.bind.is_private or args.bind.is_unspecified:
        parser.error("--bind must be a specific private IPv4 address")
    if not 1 <= args.port <= 65535:
        parser.error("--port must be between 1 and 65535")

    from faster_whisper import WhisperModel

    print(f"Loading {args.model} locally...", flush=True)
    model = WhisperModel(args.model, device="cpu", compute_type="int8", cpu_threads=4)
    with Companion((str(args.bind), args.port), Request) as server:
        server.model = model
        print(f"Voice companion ready on {args.bind}:{args.port}.", flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
