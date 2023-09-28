let fs = require("fs-extra");
let cp = require("child_process");
let util = require("util");
let WebSocket = require("ws");
let fetch = require("node-fetch");
let request = require("request-promise");
const { EventEmitter } = require("events");
const path = require("path");
//process.env.NODE_TLS_REJECT_UNAUTHORIZED = "0";

const chokidar = require("chokidar");
const IS_WIN = process.platform === "win32";
const IS_MAC = process.platform === "darwin";

class api extends EventEmitter {
  constructor(executablePath) {
    super();
    this.client = {
      name: "",
      pid: "",
      port: "",
      user: "riot",
      token: "",
      protocol: "",
      auth: "",
      address: "localhost",
    };

    this.summoner = {
      accountId: "",
      displayName: "",
      internalName: "",
      percentCompleteForNextLevel: "",
      profileIconId: "",
      puuid: "",
      summonerId: "",
      summonerLevel: "",
      xpSinceLastLevel: "",
      xpUntilNextLevel: "",
    };

    if (executablePath) {
      this._dirPath = path.dirname(path.normalize(executablePath));
    }
  }

  static getLCUPathFromProcess() {
    return new Promise((resolve) => {
      const INSTALL_REGEX_WIN = /"--install-directory=(.*?)"/;
      const INSTALL_REGEX_MAC = /--install-directory=(.*?)( --|\n|$)/;
      const INSTALL_REGEX = IS_WIN ? INSTALL_REGEX_WIN : INSTALL_REGEX_MAC;
      const command = IS_WIN
        ? `WMIC PROCESS WHERE name='LeagueClientUx.exe' GET commandline`
        : `ps x -o args | grep 'LeagueClientUx'`;

      cp.exec(command, (err, stdout, stderr) => {
        if (err || !stdout || stderr) {
          resolve();
          return;
        }
        const parts = stdout.match(INSTALL_REGEX) || [];
        resolve(parts[1]);
      });
    });
  }

  static isValidLCUPath(dirPath) {
    if (!dirPath) {
      return false;
    }

    const lcuClientApp = IS_MAC ? "LeagueClient.app" : "LeagueClient.exe";
    const common =
      fs.existsSync(path.join(dirPath, lcuClientApp)) &&
      fs.existsSync(path.join(dirPath, "Config"));
    const isGlobal = common && fs.existsSync(path.join(dirPath, "RADS"));
    const isCN = common && fs.existsSync(path.join(dirPath, "TQM"));
    const isGarena = common; // Garena has no other

    return isGlobal || isCN || isGarena;
  }

  start() {
    console.log("started");
    // if (api.isValidLCUPath(this._dirPath)) {
    //     this._initLockfileWatcher();
    //     return;
    // }
    this._initProcessWatcher();
  }

  stop() {
    console.log("stopped");
    this._clearProcessWatcher();
    this._clearLockfileWatcher();
  }

  getClient() {
    return this.client;
  }

  _initLockfileWatcher() {
    if (this._lockfileWatcher) {
      return;
    }
    const lockfilePath = path.join(this._dirPath, "lockfile");
    this._lockfileWatcher = chokidar.watch(lockfilePath, {
      disableGlobbing: true,
    });

    this._lockfileWatcher.on("add", this._onFileCreated.bind(this));
    this._lockfileWatcher.on("change", this._onFileCreated.bind(this));
    this._lockfileWatcher.on("unlink", this._onFileRemoved.bind(this));
  }

  _clearLockfileWatcher() {
    if (this._lockfileWatcher) {
      this._lockfileWatcher.close();
    }
  }

  _initProcessWatcher() {
    return api.getLCUPathFromProcess().then((lcuPath) => {
      if (lcuPath) {
        this._dirPath = lcuPath;
        this._clearProcessWatcher();
        this._initLockfileWatcher();
        return;
      }

      if (!this._processWatcher) {
        this._processWatcher = setInterval(
          this._initProcessWatcher.bind(this),
          1000
        );
      }
    });
  }

  _clearProcessWatcher() {
    clearInterval(this._processWatcher);
  }

  _onFileCreated(path) {
    fs.readFile(path, "utf8")
      .then((data) => {
        [
          this.client.name,
          this.client.pid,
          this.client.port,
          this.client.token,
          this.client.protocol,
        ] = data.split(":");
        this.client.auth =
          "Basic " +
          Buffer.from(`riot:${this.client.token}`).toString("base64");
        this._initWebSocket();
        this._initRequest();
        this._initSummoner()
          .then(() => this.emit("connect", this.client))
          .catch((e) => {});
      })
      .catch((e) => {
        console.error(e);
      });
  }

  _onFileRemoved() {
    this.emit("disconnect");
  }

  _initWebSocket() {
    let wsUrl = `wss://riot:${this.client.token}@127.0.0.1:${this.client.port}`;
    console.log("socket", wsUrl);
    let socket = new WebSocket(wsUrl, {
      headers: {
        Authorization: this.client.auth,
      },
      rejectUnauthorized: false,
    });
    socket.on("open", () => {
      socket.send(JSON.stringify([5, "OnJsonApiEvent"]), null, null);
    });

    socket.on("message", (data) => {
      this.emit("message", data);
    });
    socket.on("error", (data) => {
      console.log("error", data);
      this.emit("error", data);
    });
  }

  _initRequest() {
    request = request.defaults({
      headers: {
        Authorization: this.client.auth,
      },
      strictSSL: false,
      json: true,
    });
  }

  _initSummoner() {
    return new Promise((resolve, reject) => {
      this.get("/lol-summoner/v1/current-summoner")
        .then((res) => {
          this.summoner.accountId = res.accountId;
          this.summoner.displayName = res.displayName;
          this.summoner.internalName = res.internalName;
          this.summoner.percentCompleteForNextLevel =
            res.percentCompleteForNextLevel;
          this.summoner.profileIconId = res.profileIconId;
          this.summoner.puuid = res.puuid;
          this.summoner.summonerId = res.summonerId;
          this.summoner.summonerLevel = res.summonerLevel;
          this.summoner.xpSinceLastLevel = res.xpSinceLastLevel;
          this.summoner.xpUntilNextLevel = res.xpUntilNextLevel;
          console.log(this.summoner);
          resolve("ok");
        })
        .catch((e) => {
          //console.error(e);
          reject("notok");
        });
    });
  }

  get(url) {
    return new Promise((resolve, reject) => {
      let link = `${this.client.protocol}://127.0.0.1:${this.client.port}`;
      url[0] !== "/" ? (link += "/" + url) : (link += url);
      //console.log("GET: ",link);
      request
        .get(link)
        .then((res) => resolve(res))
        .catch((err) => {
          console.error("get err: ", err.message);
          // if(err.message.includes("You are not logged in")){
          //     console.log("Please wait 10s ");
          //     this.stop();
          //     setTimeout(()=>{
          //         process.kill(process.pid, 'SIGTERM');
          //     }, 10000);
          // }
          reject(err.message);
        });
    });
  }

  post(url, data) {
    return new Promise((resolve, reject) => {
      let link = `${this.client.protocol}://127.0.0.1:${this.client.port}`;
      url[0] !== "/" ? (link += "/" + url) : (link += url);
      console.log("POST: ", link);
      if (!data) data = "";
      request
        .post(link, { body: data })
        .then((res) => resolve(res))
        .catch((err) => {
          console.error(err.message);
          reject(err.message);
        });
    });
  }

  put(url, data) {
    return new Promise((resolve, reject) => {
      let link = `${this.client.protocol}://127.0.0.1:${this.client.port}`;
      url[0] !== "/" ? (link += "/" + url) : (link += url);
      console.log("POST: ", link);
      if (!data) data = "";
      request
        .put(link, { body: data })
        .then((res) => resolve(res))
        .catch((err) => {
          console.error(err.message);
          reject(err.message);
        });
    });
  }

  patch(url, data) {
    if (!data) data = "";
    return new Promise((resolve, reject) => {
      let link = `${this.client.protocol}://127.0.0.1:${this.client.port}`;
      url[0] !== "/" ? (link += "/" + url) : (link += url);
      //console.log("PATCH: ",link);
      let option = {
        uri: link,
        json: true,
        method: "PATCH",
        body: data,
      };
      //console.log(option);
      request
        .patch(option)
        .then((res) => resolve(res))
        .catch((err) => {
          console.error(err.message);
          reject(err.message);
        });
    });
  }

  getChampionName() {
    return new Promise((resolve, reject) => {
      this.get("/lol-chat/v1/me")
        .then((res) => {
          if (res.name) resolve(res.name);
        })
        .catch((e) => {
          console.error(e);
          reject(e);
        });
    });
  }

  getCurentSummor() {
    return this.summoner;
  }

  getListChampions(idsummoner) {
    return new Promise((resolve, reject) => {
      if (!idsummoner) idsummoner = this.summoner.summonerId;
      this.get("/lol-champions/v1/inventories/" + idsummoner + "/champions")
        .then((res) => {
          try {
            let own = [];
            let list = res;
            list.forEach((onechampion) => {
              if (onechampion.ownership.owned || onechampion.freeToPlay) {
                own.push(onechampion.id + " " + onechampion.name);
              }
            });
            resolve(own);
          } catch (e) {
            console.error(e);
            reject(e);
          }
        })
        .catch((e) => {
          console.error(e);
          reject(e);
        });
    });
  }
}

module.exports = {
  api,
};
