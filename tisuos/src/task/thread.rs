//! # 线程（有空再做）
//! 线程是进程的缩减版，但是不能独立存在，必须依赖于进程
//! 线程只保留自身的寄存器、栈环境
//! 
//! 2021年1月1日 zg

#![allow(dead_code)]

extern "C" {
    fn thread_exit();
    fn process_exit();
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum ThreadState{
    Running,
    Waiting,
    Sleeping,
}

impl ThreadState {
    pub fn to_task_state(&self)->TaskState {
        match self {
            ThreadState::Running => {TaskState::Running}
            ThreadState::Waiting => {TaskState::Waiting}
            ThreadState::Sleeping => {TaskState::Sleeping}
        }
    }

    pub fn from_task_state(state : TaskState)->Self {
        match state {
            TaskState::Running => {Self::Running}
            TaskState::Waiting => {Self::Waiting}
            TaskState::Sleeping => {Self::Sleeping}
        }
    }
}

/// ## 线程
/// 不保存堆信息，pid 记录所属进程号
pub struct Thread{
    pub env : Environment,
    pub state : ThreadState,
    pub stack_top : *mut u8,
    pub pid : usize,
    pub tid : usize,
    pub is_kernel : bool,
}

/// ## 线程功能
/// fork 拷贝原线程栈内容然后创建新的分支
/// branch 直接根据传入地址（默认是原线程的第二个参数）创建新的线程
impl Thread {
    pub fn new(func : usize, p : &Process)->Option<Self>{
        let mut env = Environment::new();
        let stack_bottom;
        let tid = unsafe{ THREAD_CNT };
        let stack_top;
        if let Some(st) = Thread::stack(p.is_kernel) {
            stack_bottom = st;
        }
        else {
            return None;
        }
        let satp = SATP::from(p.info.satp);
        let pt = satp.get_page_table().unwrap();
        page_table::map_kernel_area(pt, p.is_kernel);
        Thread::map_stack(pt, stack_bottom, p.is_kernel);

        stack_top = stack_bottom as usize + PAGE_SIZE * STACK_PAGE_NUM;
        env.epc = func;
        env.satp = p.info.satp;
        env.regs[Register::A0.val()] = tid;
        env.regs[Register::SP.val()] = stack_top;
        env.regs[Register::RA.val()] = process_exit as usize;
        unsafe {
            THREAD_CNT = THREAD_CNT + 1;
            if THREAD_CNT == 0{
                THREAD_CNT += 1;
            }
        }
        Some(Self{
            env : env,
            state : ThreadState::Waiting,
            stack_top : stack_top as *mut u8,
            pid : p.info.pid,
            tid : tid,
            is_kernel : p.is_kernel,
        })
    }
    
    pub fn fork(src_th : &Thread)->Option<Self>{
        let mut env = src_th.env;
        let stack_bottom;
        let tid = unsafe{ THREAD_CNT };
        let stack_top;
        if let Some(st) = Thread::stack(src_th.is_kernel) {
            stack_bottom = st;
        }
        else {
            return None;
        }
        let satp = SATP::from(src_th.env.satp);
        let pt = satp.get_page_table().unwrap();
        Thread::map_stack(pt, stack_bottom, src_th.is_kernel);
        unsafe {
            let stack_size = STACK_PAGE_NUM * PAGE_SIZE;
            let src = (src_th.stack_top as usize - stack_size) as *mut u8;
            stack_bottom.copy_from(src, stack_size);
        }

        stack_top = stack_bottom as usize + PAGE_SIZE * STACK_PAGE_NUM;
        env.epc = src_th.env.epc + 4;
        env.regs[Register::SP.val()] = stack_top - (src_th.stack_top as usize - src_th.env.regs[Register::SP.val()]);
        // println!("thread fork src tid {} stack {:x} sp {:x}, new stack {:x} sp {:x} tid {}",
        // src_th.tid, src_th.stack_top as usize, src_th.env.regs[Register::SP.val()], stack_top,
        //     env.regs[Register::SP.val()], tid);
        env.regs[Register::A0.val()] = tid;
        unsafe {
            THREAD_CNT = THREAD_CNT + 1;
            if THREAD_CNT == 0{
                THREAD_CNT += 1;
            }
        }
        Some(Self{
            env : env,
            state : ThreadState::Waiting,
            stack_top : stack_top as *mut u8,
            pid : src_th.pid,
            tid : tid,
            is_kernel : src_th.is_kernel,
        })
    }

    pub fn branch(src_th : &Thread)->Option<Self>{
        let mut env = src_th.env;
        let stack_bottom;
        let tid = unsafe{ THREAD_CNT };
        let stack_top;
        if let Some(st) = Thread::stack(src_th.is_kernel) {
            stack_bottom = st;
        }
        else {
            return None;
        }
        let satp = SATP::from(src_th.env.satp);
        let pt = satp.get_page_table().unwrap();
        Thread::map_stack(pt, stack_bottom, src_th.is_kernel);

        stack_top = stack_bottom as usize + PAGE_SIZE * STACK_PAGE_NUM;
        env.epc = src_th.env.regs[Register::A1.val()];
        println!("thread branch src tid {} stack {:x} sp {:x}, new stack {:x} sp {:x} tid {}",
        src_th.tid, src_th.stack_top as usize, src_th.env.regs[Register::SP.val()], stack_top,
            env.regs[Register::SP.val()], tid);
        env.regs[Register::A0.val()] = tid;
        unsafe {
            THREAD_CNT = THREAD_CNT + 1;
            if THREAD_CNT == 0{
                THREAD_CNT += 1;
            }
        }
        Some(Self{
            env : env,
            state : ThreadState::Waiting,
            stack_top : stack_top as *mut u8,
            pid : src_th.pid,
            tid : tid,
            is_kernel : src_th.is_kernel,
        })
    }

    fn stack(is_kernel : bool)->Option<*mut u8> {
        if is_kernel {
            alloc_kernel_page(STACK_PAGE_NUM)
        }
        else {
            alloc_user_page(STACK_PAGE_NUM)
        }
    }

    fn map_stack(pt : &mut PageTable, stack_bottom : *mut u8, is_kernel : bool) {
        if is_kernel{
            for i in 0..STACK_PAGE_NUM{
                let addr = stack_bottom as usize + i * PAGE_SIZE;
                pt.map_kernel_data(addr, addr);
            }
        }
        else {
            for i in 0..STACK_PAGE_NUM{
                let addr = stack_bottom as usize + i * PAGE_SIZE;
                pt.map_user_data(addr, addr);
            }
        }
    }
}

/// ## 信息相关操作
impl Thread {
    pub fn get_exec_info(&self)->ExecutionInfo {
        ExecutionInfo {
            pid: self.pid,
            tid: self.tid,
            state: self.state.to_task_state(),
            is_kernel: self.is_kernel,
            stack_top: self.stack_top,
            env: self.env,
        }
    }

    pub fn set_exec_info(&mut self, info : &ExecutionInfo) {
        self.state = ThreadState::from_task_state(info.state);
        self.stack_top = info.stack_top;
        self.pid = info.pid;
        self.tid = info.tid;
        self.env = info.env;
    }

    pub fn save(&mut self, env : &Environment) {
        self.env = env.clone();
    }
}

impl Drop for Thread{
    fn drop(&mut self) {
        let stack_bottom = self.stack_top as usize - PAGE_SIZE * STACK_PAGE_NUM;
        free_page(stack_bottom as *mut u8);
        delete_pipe(self.tid);
    }
}

static mut THREAD_CNT : usize = 1;

/// ## 初始化临时环境内存
pub fn init(){
}


use crate::{interrupt::trap::{Environment, Register}, memory::{alloc_kernel_page,
    alloc_user_page, config::PAGE_SIZE, free_page, page_table::{self, PageTable, SATP}},
    uart};

use super::{delete_pipe, task_info::{ExecutionInfo}, process::{Process, STACK_PAGE_NUM},
    task_info::TaskState};



