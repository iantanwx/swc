var _class, _descriptor, _dec, _dec1;
enum MyEnum {
    x = "xxx",
    y = "yyy"
}
let Xpto = ((_class = class Xpto {
    constructor(){
        _initializerDefineProperty(this, "value", _descriptor, this);
    }
}) || _class, _dec = Decorator(), _dec1 = typeof Reflect !== "undefined" && typeof Reflect.metadata === "function" && Reflect.metadata("design:type", String), _descriptor = _applyDecoratedDescriptor(_class.prototype, "value", [
    _dec,
    _dec1
], {
    configurable: true,
    enumerable: true,
    writable: true,
    initializer: null
}), _class);
function Decorator() {
    return function(...args) {};
}
