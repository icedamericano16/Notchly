"""OBS Studio script (Tools › Scripts): shows a red REC live activity while recording."""
import subprocess

import obspython as obs

ID = "obs-rec"


def cli(*a):
    subprocess.Popen(["notchly", "live", *a], creationflags=0x08000000)


def on_event(event):
    if event == obs.OBS_FRONTEND_EVENT_RECORDING_STARTED:
        cli("start", "--id", ID, "--title", "Recording", "--icon", "camera", "--color", "#FF4646", "--priority", "80")
    elif event == obs.OBS_FRONTEND_EVENT_RECORDING_STOPPED:
        cli("end", "--id", ID, "--final", "Recording saved")


def script_load(settings):
    obs.obs_frontend_add_event_callback(on_event)


def script_description():
    return "Shows OBS recording state in the Notchly notch."
