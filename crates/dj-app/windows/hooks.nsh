;; djmanzo's Windows installer hooks (Tauri `bundle > windows > nsis >
;; installerHooks`).
;;
;; §122: the owner asked for karaoke's word-timing model to be "downloadable
;; from the installer" where it can be. This is the one package that can: after
;; installing, it asks whether to download the recommended Whisper model
;; (Small, 190 MB) and fetches it into the folder djmanzo keeps its models in,
;; checked against its published SHA-256 — the same file, the same place and
;; the same checksum as `dj_app::whispercpp::MODELS`, which a test holds this
;; file to. Declined, silent or failed, nothing is lost: djmanzo offers it
;; again at first run and in its Singers panel.

!macro NSIS_HOOK_POSTINSTALL
  IfSilent djmanzo_words_done
  MessageBox MB_YESNO|MB_ICONQUESTION "djmanzo can time the words of karaoke songs so the singers' screen wipes the lyrics as they are sung. That needs one model, Whisper Small (190 MB), downloaded once.$\r$\n$\r$\nDownload it now?" IDNO djmanzo_words_done
  DetailPrint "Downloading Whisper Small (190 MB) for karaoke word timing..."
  nsExec::ExecToLog `powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$ErrorActionPreference='Stop'; $$ProgressPreference='SilentlyContinue'; [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; $$dir=Join-Path $$env:APPDATA 'app.djmanzo.desktop\models\whisper'; New-Item -ItemType Directory -Force -Path $$dir | Out-Null; $$to=Join-Path $$dir 'ggml-small-q5_1.bin'; $$part=$$to+'.part'; Invoke-WebRequest -UseBasicParsing -Uri 'https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-small-q5_1.bin' -OutFile $$part; if ((Get-FileHash $$part -Algorithm SHA256).Hash -ne 'ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb') { Remove-Item $$part; exit 1 }; Move-Item -Force $$part $$to"`
  Pop $0
  StrCmp $0 "0" djmanzo_words_done
  MessageBox MB_OK|MB_ICONINFORMATION "The model could not be downloaded now. djmanzo offers it again when it starts, and in its Singers panel."
  djmanzo_words_done:
!macroend
