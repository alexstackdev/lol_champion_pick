console.clear();
// let height = [0, 1, 0, 2, 1, 0, 1, 3, 2, 1, 2, 1];
let height = [4, 2, 0, 8, 2, 8, 0, 0, 0, 9];

const run = (arr) => {
  let x = [...arr].sort((a, b) => a - b);
  console.log(x);
  console.log(arr.join(" "));
  let max = arr[0];
  for (let i in arr) {
    let val = arr[i];
    if (val > max) max = val;
  }
  console.log({ max });
  let resultArr = Array(max).fill("");
  for (let i of arr) {
    for (let j = 0; j < max; j++) {
      if (i > j) {
        resultArr[j] += "1";
      } else {
        resultArr[j] += "0";
      }
    }
  }
  //   console.log({ resultArr });
  resultArr.reverse().map((e) => {
    console.log(
      e
        .split("")
        .map((z) => (z === "1" ? "\u2617" : `\u2616`))
        .join("")
    );
  });

  let count = 0;
  resultArr.map((e) => {
    let re = String(e).match(/1(0*1)+/gm);
    if (!re) return;
    // console.log(re);
    count += String(re[0]).replaceAll("1", "").length;
  });
  console.log(count);
};

run(height);
