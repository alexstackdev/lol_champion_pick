function func() {
    return new Promise(((resolve, reject) => {
        resolve("1");
    }))
}


func()
.then(e=>{console.log(e)
    return Promise.resolve("2")
})
.then(q=>{
    console.log(q)
    let a={
        asdad:"asdasd",
        aaa:"cc"
    }
    return Promise.resolve(a)
})
.then(z=>{
    console.log(z);
})