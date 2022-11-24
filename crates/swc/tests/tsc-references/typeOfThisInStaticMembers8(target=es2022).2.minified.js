//// [typeOfThisInStaticMembers8.ts]
class C {
    static f = 1;
    static arrowFunctionBoundary = ()=>this.f + 1;
    static functionExprBoundary = function() {
        return this.f + 2;
    };
    static classExprBoundary = class {
        a = this.f + 3;
    };
    static functionAndClassDeclBoundary = void 0;
}
