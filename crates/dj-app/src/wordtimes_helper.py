"""djmanzo's helper for WhisperX: when each word of a record is sung.

Run by djmanzo (see dj_app::wordtimes) inside the private environment it
installed WhisperX into. A job arrives as JSON on standard input; the answer
leaves as one line of JSON on standard output; anything on standard error is
progress, and its last line is the reason when this fails.

The audio arrives already decoded -- a sixteen-kilohertz mono 16-bit WAV --
and is read here with the standard library, so WhisperX never needs FFmpeg,
which it otherwise calls to read files and which a Mac or a Windows machine
does not have.

With the words given (`lines`), only WhisperX's aligner runs: each known line
is placed word by word. With none, the record is transcribed first.
"""

import json
import sys
import time
import wave


def say(text):
    print(text, file=sys.stderr, flush=True)


def load(path):
    import numpy as np

    with wave.open(path, "rb") as audio:
        if audio.getframerate() != 16000 or audio.getnchannels() != 1 or audio.getsampwidth() != 2:
            raise SystemExit("the audio is not sixteen-kilohertz mono 16-bit")
        frames = audio.readframes(audio.getnframes())
    return np.frombuffer(frames, dtype=np.int16).astype(np.float32) / 32768.0


def main():
    job = json.load(sys.stdin)
    stages = []

    began = time.monotonic()
    say("starting WhisperX")
    import torch

    if job.get("threads"):
        torch.set_num_threads(int(job["threads"]))
    import whisperx

    audio = load(job["audio"])
    stages.append(["start", time.monotonic() - began])

    language = job.get("language") or None
    lines = job.get("lines") or []

    def whisper():
        try:
            return whisperx.load_model(
                job.get("model") or "base",
                "cpu",
                compute_type="int8",
                language=language,
                threads=int(job.get("threads") or 4),
            )
        except TypeError:
            return whisperx.load_model(job.get("model") or "base", "cpu", compute_type="int8", language=language)

    if lines:
        mode = "align"
        if not language:
            # The aligner is chosen by language. Listening for it is one
            # pass of the small model over the opening, far cheaper than
            # finding every word.
            began = time.monotonic()
            say("listening for the language")
            language = whisper().detect_language(audio)
            stages.append(["language", time.monotonic() - began])
        segments = [
            {"start": float(line["start"]), "end": float(line["end"]), "text": line["text"]}
            for line in lines
        ]
    else:
        mode = "transcribe"
        began = time.monotonic()
        say("finding the words")
        result = whisper().transcribe(audio, batch_size=8, language=language)
        segments = result["segments"]
        language = result.get("language") or language
        stages.append(["transcribe", time.monotonic() - began])
        if not segments:
            raise SystemExit("no words were heard")

    began = time.monotonic()
    say("placing each word")
    # djmanzo's choice where WhisperX's default is non-commercial (see
    # dj_app::wordtimes::ALIGNERS); WhisperX's own everywhere else.
    chosen = (job.get("aligners") or {}).get(language)
    aligner, metadata = whisperx.load_align_model(language_code=language, device="cpu", model_name=chosen)
    aligned = whisperx.align(segments, aligner, metadata, audio, "cpu", return_char_alignments=False)
    stages.append(["align", time.monotonic() - began])

    def number(value):
        return None if value is None else float(value)

    answer = {
        "mode": mode,
        "language": language,
        "segments": [
            {
                "start": number(segment.get("start")),
                "end": number(segment.get("end")),
                "text": (segment.get("text") or "").strip(),
                "words": [
                    {
                        "word": word.get("word", ""),
                        "start": number(word.get("start")),
                        "end": number(word.get("end")),
                        "score": number(word.get("score")),
                    }
                    for word in segment.get("words", [])
                ],
            }
            for segment in aligned.get("segments", [])
        ],
        "stages": stages,
    }
    print(json.dumps(answer), flush=True)


def reason(error):
    """The first line of an error that says anything: NLTK's, for one, opens
    with a row of asterisks, which as the last line said would be all the DJ
    was told."""
    for line in str(error).splitlines():
        if any(character.isalnum() for character in line):
            return line.strip()
    return ""


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        import traceback

        traceback.print_exc()
        say(f"{type(error).__name__}: {reason(error)}".rstrip(": "))
        sys.exit(1)
