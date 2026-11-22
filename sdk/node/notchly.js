// Notchly Live Activities SDK for Node.js (Windows). No dependencies.
//
//   const { live } = require("./notchly");
//   const a = await live("build-42", { title: "Build", icon: "hammer" }, (action) => console.log(action));
//   a.update({ progress: 0.5 });
//   a.end("Done in 2m 14s");
const net = require("net");

const PIPE = "\\\\.\\pipe\\notchly-live";

function live(id, fields = {}, onAction) {
  return new Promise((resolve, reject) => {
    const sock = net.connect(PIPE);
    const send = (m) => sock.write(JSON.stringify(m) + "\n");
    let buf = "";
    sock.on("data", (d) => {
      buf += d;
      let i;
      while ((i = buf.indexOf("\n")) >= 0) {
        const ev = JSON.parse(buf.slice(0, i));
        buf = buf.slice(i + 1);
        if (ev.event === "action" && onAction) onAction(ev.action);
      }
    });
    sock.on("error", reject);
    sock.on("connect", () => {
      send({ op: "start", id, ...fields });
      resolve({
        update: (f) => send({ op: "update", id, ...f }),
        end: (final = "Done", linger = 4) => { send({ op: "end", id, final, linger }); sock.end(); },
      });
    });
  });
}

module.exports = { live };
