# Heart rate protocol

The desktop app runs a small WebSocket server (default port `9555`) and
announces it in the local network via mDNS as `_osctoolhr._tcp`. Anything that
can open a WebSocket can feed it; the watch app is just one client.

Connect to `ws://<pc-address>:9555/` and send text frames:

| Frame | Meaning |
| --- | --- |
| `{"type":"hello","device":"Galaxy Watch7","app":"0.1.0"}` | Optional. Sets the device name shown in the app. |
| `{"type":"hr","bpm":72}` | A heart rate value (20–255). |
| `{"type":"hr","bpm":0}` | The sensor is running but has no reading. |
| `72` | Shorthand for an `hr` frame. |

The server answers once with `{"type":"welcome","name":"<pc name>"}`.

Send at least one frame every few seconds. Without a value for the configured
timeout (default 10 s) the heart rate counts as lost. Frames larger than 4 KB
close the connection.

Quick test without a watch:

```bash
npx wscat -c ws://127.0.0.1:9555
```

Then type `90` and press Enter.

## What is sent to VRChat

All output is OSC over UDP to `127.0.0.1:9000` (configurable).

| Address | Type | Notes |
| --- | --- | --- |
| `/chatbox/input` | string, bool, bool | Text, send immediately, notification sound |
| `/chatbox/typing` | bool | While you type a message in the app |
| `/avatar/parameters/HR` | int | Beats per minute, 0 without a reading |
| `/avatar/parameters/onesHR`, `tensHR`, `hundredsHR` | int | Single digits |
| `/avatar/parameters/HRPercent` | float 0..1 | Position inside the configured range |
| `/avatar/parameters/FullHRPercent` | float -1..1 | Same, centered |
| `/avatar/parameters/isHRConnected` | bool | A device is connected |
| `/avatar/parameters/isHRActive` | bool | A current reading exists |
| `/avatar/parameters/isHRBeat` | bool | Flashes once per heartbeat |

The parameter names follow HRtoVRChat_OSC, so existing heart rate prefabs work
unchanged.
