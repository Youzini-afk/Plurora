console.error("hello from Path B");

process.stdin.setEncoding("utf8");
let buffer = "";

process.stdin.on("data", (chunk) => {
  buffer += chunk;
  let newline;
  while ((newline = buffer.indexOf("\n")) >= 0) {
    const line = buffer.slice(0, newline).trim();
    buffer = buffer.slice(newline + 1);
    if (!line) continue;

    const request = JSON.parse(line);
    if (request.method === "package.handshake") {
      process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: request.id, result: { ready: true } }) + "\n");
    } else {
      process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: request.id, result: {} }) + "\n");
    }
  }
});
