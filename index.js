let apirequire = require("./api").api;
let lodash = require('lodash')
let api = new apirequire();
let request = require("request-promise");
const btoa = require("btoa");
let location = "";
const readline = require("readline");
const dateFormat = require("dateformat");
const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout,
});
let now = () => {
  return dateFormat(new Date(), "h:MM:ss:l");
};

let championID = 11;
let switchAuto = false;

function parseChampion(list, input) {
  let temp = null;
  list.forEach((item) => {
    if (item.split(" ")[0] === input) temp = item;
  });
  return temp;
}

function show(first) {
  console.clear();
  if (first === "off") {
    switchAuto = false;
    show();
    return;
  }
  console.log(`Hi ${api.getCurentSummor().displayName}`);
  api
    .getListChampions()
    .then((res) => {
      let id;
      if (first) {
        id = parseChampion(res, first);
        if (id) switchAuto = true;
      }
      console.clear()
      console.log("Auto state: ", switchAuto);
      console.log("Type champion id to choose champion to pick quickly");
      console.log("Type 0 to disable auto, choose any id to turn on again");

      res = lodash.chunk(res,5)
      for(let x of res){
        x = x.map(e=>e.padEnd(20,' '))
        console.log(x.join(''))
      }
      console.log("choose your champion: ")
      if (first) {
        if (!id) {
          show();
        } else {
          championID = id.toString();

          console.log("you chose: ", id);
        }
      }
    })
    .catch();
}

api.on("connect", (data) => {
  show();
});

let once = 1;
let lastrom = "";

api.on("message", function incoming(data) {
  if (!switchAuto) return;

  try {
    let parsed = JSON.parse(data);
    let js = parsed?.[2];
    if (!js || !js.uri.includes("lol-champ-select/v1/session")) return;
    if (js.data.actions[0]) {
      if (js.data.chatDetails.chatRoomName !== lastrom) {
        lastrom = js.data.chatDetails.chatRoomName;
        console.log("pickd");
        let sumId = api.getCurentSummor().summonerId;
        let temp;
        js.data.myTeam.forEach((item) => {
          if (item.summonerId === sumId) temp = item;
        });
        let action;
        js.data.actions[0].forEach((item) => {
          if (item.actorCellId === temp.cellId) action = item;
        });

        action.championId = parseInt(championID);
        action.completed = false;
        //console.log(action);
        setTimeout(() => {
          api
            .patch("/lol-champ-select/v1/session/actions/" + action.id, action)
            .then((res) => {
              if (res === undefined) console.log("picked, good luck");
            })
            .catch((e) => console.error(e));
        }, 50);
      }
    }
  } catch (e) {
    console.error(e);
  }
});

rl.on("line", (line) => {
  //console.log(line)
  if (line.startsWith(".")) {
    //api.get(line.substring(1, line.length)).then(res=>console.log(res)).catch(e=>{});
  }
  if (line.startsWith("/")) {
    //api.post(line.substring(1, line.length)).then(res=> console.log(res)).catch(e=>{});
  }

  if (!isNaN(line)) {
    if (line.toString() === "0") {
      show("off");
    } else if (line !== 0) show(line);
  }
});

api.start();
