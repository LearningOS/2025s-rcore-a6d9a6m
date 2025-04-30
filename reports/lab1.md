# for ch3

## 1.正确进入 U 态后，程序的特征还应有：使用 S 态特权指令，访问 S 态寄存器后会报错。 请同学们可以自行测试这些内容（运行 三个 bad 测例 (ch2b_bad_*.rs) ）， 描述程序出错行为，同时注意注明你使用的 sbi 及其版本。


## 2.深入理解 trap.S 中两个函数 __alltraps 和 __restore 的作用，并回答如下问题:


### 1.L40：刚进入 __restore 时，sp 代表了什么值。请指出 __restore 的两种使用情景。
答：刚进入时，sp为当前程序的值，restore
### 2.L43-L48：这几行汇编代码特殊处理了哪些寄存器？这些寄存器的的值对于进入用户态有何意义？请分别解释。
```
ld t0, 32*8(sp)
ld t1, 33*8(sp)
ld t2, 2*8(sp)
csrw sstatus, t0
csrw sepc, t1
csrw sscratch, t2
```
### 3.L50-L56：为何跳过了 x2 和 x4？
```
ld x1, 1*8(sp)
ld x3, 3*8(sp)
.set n, 5
.rept 27
   LOAD_GP %n
   .set n, n+1
.endr
```
x2之后保存 ， x4程序不用
### 4.L60：该指令之后，sp 和 sscratch 中的值分别有什么意义？
```
csrrw sp, sscratch, sp
```
答：交换了两者的值

### 5.__restore：中发生状态切换在哪一条指令？为何该指令执行之后会进入用户态？
csrrw sp, sscratch, sp这一句，因为交换了用户态的栈和内核栈


### 6.L13：该指令之后，sp 和 sscratch 中的值分别有什么意义？
```
csrrw sp, sscratch, sp
```
交换后
sp ->内核栈  ,sscratch -> 用户栈
### 7.从 U 态进入 S 态是哪一条指令发生的？
csrrw sp, sscratch, sp

